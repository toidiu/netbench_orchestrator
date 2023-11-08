[[ -z "$1" ]] && { echo "Please specify an 'id'" ; exit 1; }

id=$1

mkdir -p test_output

ctr=1
pushd test_output

    touch $id
    while :
    do
        echo $ctr >> $id
        sleep 1
        ctr=$((ctr+1))
    done

popd test_output

